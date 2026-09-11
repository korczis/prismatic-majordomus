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

/// Where one requirement stands.
///
/// Three states, and the third is the reason there are not two: a waiver is *reported* and
/// never counted as covered, so a capability whose descriptor excuses it from being
/// benchmarked still appears in the denominator with its typed reason beside it. A
/// requirement that quietly became covered by being waived is exactly what this
/// distinction refuses.
///
/// ```
/// use majordomus_cli::bench::CoverageState;
/// // the three are distinct on the wire, so a document cannot blur them
/// let words: Vec<String> = [CoverageState::Covered, CoverageState::Missing,
///                           CoverageState::Waived]
///     .iter()
///     .map(|s| serde_json::to_value(s).unwrap().to_string())
///     .collect();
/// assert_eq!(words, ["\"covered\"", "\"missing\"", "\"waived\""]);
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
/// The unit of the denominator is a *pair*, not a capability: a capability exposed over
/// three transports is three requirements, because being fast in process says nothing
/// about the socket. `cases` is carried so that a covered line says how much evidence is
/// behind it, and `reason` is present only for a waiver — a line with a reason and any
/// other state would be a document contradicting itself.
///
/// ```
/// use majordomus_cli::bench::{CoverageLine, CoverageState, Transport};
/// let line: CoverageLine = serde_json::from_value(serde_json::json!({
///     "subject": "demo.echo", "module": "demo", "transport": "http",
///     "state": "missing", "cases": 0, "reason": null
/// })).unwrap();
/// assert_eq!(line.transport, Transport::Http);
/// assert_eq!(line.state, CoverageState::Missing);
/// // required, and no case could be produced for this repository
/// assert_eq!(line.cases, 0);
/// assert!(line.reason.is_none(), "only a waiver carries a reason");
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

/// The coverage document: every requirement, and the tallies derived from them.
///
/// The tallies are computed from the lines and never recorded beside them, so a count and
/// a list that disagree is not a state this document can be in. There is one bucket per
/// transport, one called `system` for the transports' own targets, and `total` — which is
/// why no capability module may be called `system`: its lines would be tallied as transport
/// targets and vanish from the per-transport denominators.
///
/// ```
/// use majordomus_cli::bench::Coverage;
/// let document: Coverage = serde_json::from_value(serde_json::json!({
///     "schema": "majordomus/benchmark-coverage/v1",
///     "lines": [{ "subject": "demo.echo", "module": "demo", "transport": "direct",
///                 "state": "covered", "cases": 2, "reason": null },
///               { "subject": "demo.echo", "module": "demo", "transport": "http",
///                 "state": "missing", "cases": 0, "reason": null }],
///     "tallies": { "direct": { "required": 1, "covered": 1, "missing": 0, "waived": 0 },
///                  "http": { "required": 1, "covered": 0, "missing": 1, "waived": 0 },
///                  "total": { "required": 2, "covered": 1, "missing": 1, "waived": 0 } }
/// })).unwrap();
/// // one capability over two transports is two requirements, not one
/// assert_eq!(document.lines.len(), 2);
/// assert_eq!(document.tallies["total"].required, 2);
/// assert!(!document.has_no_missing(), "a transport with no target is a gap");
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
    /// Compute coverage for a context: the denominator from the registry, the numerator
    /// from the projection.
    ///
    /// Both halves are derived, which is the whole design. The requirements are every
    /// executable entry crossed with the transports its own exposure declares — so a
    /// capability that gains an HTTP route gains a requirement the same day — and a
    /// requirement is covered when the projection actually holds a target for it. Nothing
    /// is written down, so nothing can be forgotten from the denominator; a capability with
    /// no case is `Missing` rather than absent.
    ///
    /// ```no_run
    /// use majordomus_cli::bench::{BenchmarkProjection, Coverage};
    /// use majordomus_cli::capability::Context;
    /// // compiled and not run: both halves are derived from this process's own registry
    /// fn measure(ctx: &Context) {
    ///     let projection = BenchmarkProjection::from_context(ctx);
    ///     let coverage = Coverage::compute(ctx, &projection);
    ///     // the tallies are derived from the lines, so they cannot disagree with them
    ///     assert_eq!(coverage.tallies["total"].required, coverage.lines.len());
    ///     // and the transports' own targets are covered by construction
    ///     assert_eq!(coverage.tallies["system"].missing, 0);
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
    /// Stricter than [`Coverage::has_no_missing`] on purpose, and the difference is the
    /// whole reason both exist: a waiver is a decision somebody made, so a repository can
    /// be *passing* with waivers and still not be *complete*. A gate uses the looser
    /// question; a report that says whether the debt is gone uses this one.
    ///
    /// ```
    /// # use majordomus_cli::bench::Coverage;
    /// # fn document(missing: usize, waived: usize) -> Coverage {
    /// #     serde_json::from_value(serde_json::json!({
    /// #         "schema": "majordomus/benchmark-coverage/v1",
    /// #         "lines": [],
    /// #         "tallies": { "total": { "required": 3, "covered": 1,
    /// #             "missing": missing, "waived": waived } }
    /// #     })).unwrap()
    /// # }
    /// assert!(document(0, 0).is_complete());
    /// // a waiver is reported, not absorbed: it is still not complete
    /// let waived = document(0, 1);
    /// assert!(!waived.is_complete());
    /// assert!(waived.has_no_missing(), "and yet nothing is missing");
    /// assert!(!document(1, 0).is_complete());
    /// ```
    pub fn is_complete(&self) -> bool {
        let total = self.tallies.get("total").cloned().unwrap_or_default();
        total.missing == 0 && total.waived == 0
    }

    /// Nothing missing; waivers reported but not failing.
    ///
    /// The question a gate asks. A waiver was a deliberate decision recorded in a
    /// descriptor and does not fail a build; a requirement nobody has produced a case for
    /// is work that has not been done.
    ///
    /// A document with no tallies at all answers `true`, which is the honest reading of a
    /// repository that requires nothing: there is no requirement to be missing.
    ///
    /// ```
    /// # use majordomus_cli::bench::Coverage;
    /// # fn document(missing: usize, waived: usize) -> Coverage {
    /// #     serde_json::from_value(serde_json::json!({
    /// #         "schema": "majordomus/benchmark-coverage/v1",
    /// #         "lines": [],
    /// #         "tallies": { "total": { "required": 3, "covered": 1,
    /// #             "missing": missing, "waived": waived } }
    /// #     })).unwrap()
    /// # }
    /// assert!(document(0, 0).has_no_missing());
    /// assert!(document(0, 2).has_no_missing(), "a waiver does not fail a gate");
    /// assert!(!document(1, 0).has_no_missing());
    /// ```
    pub fn has_no_missing(&self) -> bool {
        self.tallies.get("total").is_none_or(|t| t.missing == 0)
    }

    /// The report a person reads: a line per bucket, the totals, and then every
    /// requirement that is not covered.
    ///
    /// Only the uncovered lines are listed, because a list of everything that is fine is a
    /// list nobody reads to the end — and the ones that are listed carry their waiver's
    /// reason, so the report answers "why is this excused?" without anybody opening a
    /// descriptor.
    ///
    /// ```
    /// use majordomus_cli::bench::Coverage;
    /// let document: Coverage = serde_json::from_value(serde_json::json!({
    ///     "schema": "majordomus/benchmark-coverage/v1",
    ///     "lines": [{ "subject": "demo.echo", "module": "demo", "transport": "http",
    ///                 "state": "waived", "cases": 0, "reason": "measured end to end" }],
    ///     "tallies": { "http": { "required": 1, "covered": 0, "missing": 0, "waived": 1 },
    ///                  "total": { "required": 1, "covered": 0, "missing": 0, "waived": 1 } }
    /// })).unwrap();
    /// let report = document.render();
    /// // the totals, and the one line that is not covered, with the reason it is not
    /// assert!(report.contains("total"), "{report}");
    /// assert!(report.contains("WAIVED"), "{report}");
    /// assert!(report.contains("measured end to end"), "{report}");
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
