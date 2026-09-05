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

/// The coverage document.
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
    /// Compute coverage for a context.
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
                module: "system".into(),
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
            let bucket = if line.module == "system" {
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
    pub fn is_complete(&self) -> bool {
        let total = self.tallies.get("total").cloned().unwrap_or_default();
        total.missing == 0 && total.waived == 0
    }

    /// Nothing missing; waivers reported but not failing.
    pub fn has_no_missing(&self) -> bool {
        self.tallies.get("total").is_none_or(|t| t.missing == 0)
    }

    /// The human report.
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
