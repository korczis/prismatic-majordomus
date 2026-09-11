//! The `evidence` module: from a claim to the run that proves it, and back.
//!
//! `docs/CLAIMS.yaml` binds a claim to a test by path. [`crate::evidence`] binds that path
//! to the runs actually recorded against it. These capabilities are the one declaration of
//! that join, and every surface — the command line, the HTTP API and its OpenAPI operation,
//! the MCP tool a client reads — is derived from it. There is no second evidence model
//! behind any of them.
//!
//! # Both directions
//!
//! A relation that can only be walked one way is half a relation. `evidence.claim` answers
//! "what proves this claim"; `evidence.test` answers "what does this test prove", listing
//! every claim that names it. They read the same derivation, so the two answers cannot
//! disagree.
//!
//! # Why the recorder is here and not exposed over the network
//!
//! `evidence.record` writes a tracked file, and this executable's MCP and HTTP surfaces are
//! read-only — that is a property of the server, not an accident of what has been built.
//! So it declares a command line and nothing else, which
//! [`Visibility::Developer`](crate::capability::Visibility) is exactly the word for: offered
//! to whoever runs the executable and to nobody over a network. Declaring it as a capability
//! rather than as a hand-written command is what keeps it inside the registry, the command
//! graph and the generated reference instead of in the inventory of commands nothing derives.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CachePolicy, CapabilityKind, CliExposure, Exposure, McpResource, Stability,
    WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::evidence::{
    self, ClaimProof, EvidenceReport, Execution, Ledger, Origin, ProofState, RecordRequest,
};
use crate::{capability, module};

use super::{get, mcp};

/// The URI under which the whole evidence report is read as an MCP resource.
pub const EVIDENCE_URI: &str = "majordomus://evidence";

// ---------------------------------------------------------------- inputs

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which part of the matrix to answer for.
pub struct EvidenceReportInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only claims in this proof state (`proven`, `inputs_unchanged`, `stale`, `failing`,
    /// `not_run`, `unrunnable`, `no_test`). Absent: every claim.
    pub state: Option<ProofState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only claims declaring this status (`guaranteed`, `advisory`, `planned`, `rejected`).
    /// Absent: every claim.
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    /// Only the claims whose declared status the evidence does not support, with the
    /// findings. The tallies still count the whole matrix, so a filtered answer never
    /// misreports how much of it was examined.
    pub findings_only: bool,
}

impl BenchmarkCases for EvidenceReportInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", EvidenceReportInput::default()),
            NamedCase::new(
                "findings",
                EvidenceReportInput {
                    findings_only: true,
                    ..Default::default()
                },
            ),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// One claim of the matrix.
pub struct EvidenceClaimInput {
    /// The claim id, as `docs/CLAIMS.yaml` spells it.
    pub claim: String,
}

impl BenchmarkCases for EvidenceClaimInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // a claim this repository actually has, so the case measures the join rather than
        // the refusal; a hard-coded id would be a second declaration of the matrix
        let claim = ctx
            .index
            .objects
            .iter()
            .find(|o| o.kind == "claim")
            .map(|o| o.identity.clone())
            .unwrap_or_else(|| "no-claim".to_string());
        vec![NamedCase::new("first-claim", EvidenceClaimInput { claim })]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// One test, by its stable identity or by the path a claim names it with.
pub struct EvidenceTestInput {
    /// `suite:84_distribution_model`, `crate:why`, or the path itself
    /// (`test/cases/84_distribution_model.sh`), which is resolved to the same identity.
    pub test: String,
}

impl BenchmarkCases for EvidenceTestInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let test = ctx
            .index
            .objects
            .iter()
            .filter(|o| o.kind == "claim")
            .find_map(|o| {
                o.metadata
                    .get("test")
                    .and_then(|v| v.as_str())
                    .and_then(evidence::TestId::of)
            })
            .map(|t| t.as_string())
            .unwrap_or_else(|| "suite:00_yaml_flatten".to_string());
        vec![NamedCase::new("first-test", EvidenceTestInput { test })]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// A run to record into the ledger.
pub struct EvidenceRecordInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The runner's TSV report: `MJ_TEST_REPORT=<file> bash test/run.sh`.
    pub suite: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A file holding `cargo test`'s output, for the crate's own integration tests.
    pub crate_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where the run happened: `local` (the default), `ci` or `release`.
    pub origin: Option<String>,
}

impl BenchmarkCases for EvidenceRecordInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // nothing: the capability is waived from the benchmark because it writes a tracked
        // file, and timing it in a loop would rewrite the repository's evidence
        Vec::new()
    }
}

// ---------------------------------------------------------------- outputs

#[derive(Debug, Clone, Serialize, JsonSchema)]
/// One claim with everything the repository can say about its proof, and the way back out
/// to the test that carries it.
pub struct ClaimEvidence {
    /// The claim, joined to its evidence.
    #[serde(flatten)]
    pub proof: ClaimProof,
    /// Every execution the ledger holds for this claim's test — today one, the latest, and
    /// a list because the ledger's retention is a decision that may change and a client
    /// that read a bare object would break when it did.
    pub executions: Vec<Execution>,
    /// The other claims the same test proves. A test shared by twelve claims is a test
    /// whose failure is twelve findings, and a reader of one of them should be able to see
    /// the other eleven without searching.
    pub also_proves: Vec<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
/// One test, and what it proves.
pub struct TestEvidence {
    /// The stable identity: `suite:<case>` or `crate:<binary>`.
    pub test: String,
    /// Which runner owns it.
    pub runner: evidence::Runner,
    /// Its own source, repository-relative.
    pub source: String,
    /// Whether that source is in this checkout.
    pub present: bool,
    /// The command that runs this one test.
    pub reproduce: String,
    /// The latest recorded execution, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<Execution>,
    /// Whether the test's source still hashes to what the execution recorded. `None` when
    /// there is no execution, or the source is not there to hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest_matches: Option<bool>,
    /// Every claim that names this test, with the state each is in.
    pub proves: Vec<ClaimProof>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
/// What a recording did.
pub struct RecordReport {
    /// How many executions were written.
    pub recorded: usize,
    /// How many of them passed.
    pub passed: usize,
    /// The commit they were recorded against.
    pub commit: String,
    /// The tree's state at the time.
    pub working_tree: String,
    /// Results the run named that no runner in this repository owns.
    pub unknown: Vec<String>,
    /// Where the ledger was written.
    pub ledger: String,
}

// ---------------------------------------------------------------- handlers

fn root_of(ctx: &Context) -> std::path::PathBuf {
    std::path::PathBuf::from(&ctx.index.repository.root)
}

fn internal(e: crate::error::Error) -> CapabilityError {
    CapabilityError::Internal(e.to_string())
}

/// The whole joined report, read fresh: the ledger is a file that changes outside this
/// process, and a cached answer would be the stale evidence this module exists to name.
fn full_report(ctx: &Context) -> Result<EvidenceReport, CapabilityError> {
    let root = root_of(ctx);
    let ledger = Ledger::load(&root).map_err(internal)?;
    Ok(evidence::report(&ctx.index, &ledger))
}

fn report(ctx: &Context, input: EvidenceReportInput) -> Result<EvidenceReport, CapabilityError> {
    let mut r = full_report(ctx)?;
    // the tallies are taken before the filter: a filtered answer says how much of the
    // matrix it looked at, never how much it returned
    if input.findings_only {
        let named: std::collections::BTreeSet<&str> =
            r.findings.iter().map(|f| f.claim.as_str()).collect();
        r.claims.retain(|c| named.contains(c.id.as_str()));
    }
    if let Some(state) = input.state {
        r.claims.retain(|c| c.state == state);
    }
    if let Some(status) = &input.status {
        r.claims.retain(|c| &c.status == status);
    }
    Ok(r)
}

fn claim(ctx: &Context, input: EvidenceClaimInput) -> Result<ClaimEvidence, CapabilityError> {
    let r = full_report(ctx)?;
    let Some(proof) = r.claims.iter().find(|c| c.id == input.claim).cloned() else {
        return Err(CapabilityError::NotFound(format!(
            "`{}` is not a claim of docs/CLAIMS.yaml",
            input.claim
        )));
    };
    let also_proves = proof
        .test
        .as_ref()
        .map(|t| {
            r.claims
                .iter()
                .filter(|c| c.test.as_ref() == Some(t) && c.id != proof.id)
                .map(|c| c.id.clone())
                .collect()
        })
        .unwrap_or_default();
    let executions = proof.execution.clone().into_iter().collect();
    Ok(ClaimEvidence {
        proof,
        executions,
        also_proves,
    })
}

fn test(ctx: &Context, input: EvidenceTestInput) -> Result<TestEvidence, CapabilityError> {
    // a path and an identity both name the same test; a client that has one should not
    // have to know the other's spelling
    let id = evidence::TestId::of(&input.test)
        .or_else(|| {
            let (prefix, name) = input.test.split_once(':')?;
            let runner = match prefix {
                "suite" => evidence::Runner::Suite,
                "crate" => evidence::Runner::Crate,
                _ => return None,
            };
            Some(evidence::TestId {
                runner,
                name: name.to_string(),
            })
        })
        .ok_or_else(|| {
            CapabilityError::InvalidInput(format!(
                "`{}` names no test: give `suite:<case>`, `crate:<binary>`, or the path of a \
                 case under test/cases/ or an integration test under apps/majordomus-cli/tests/",
                input.test
            ))
        })?;

    let root = root_of(ctx);
    let r = full_report(ctx)?;
    let key = id.as_string();
    let proves: Vec<ClaimProof> = r
        .claims
        .iter()
        .filter(|c| c.test.as_deref() == Some(key.as_str()))
        .cloned()
        .collect();
    let ledger = Ledger::load(&root).map_err(internal)?;
    let execution = ledger.latest(&key).cloned();
    let digest_matches = execution.as_ref().and_then(|e| e.digest_matches(&root));
    Ok(TestEvidence {
        test: key,
        runner: id.runner,
        source: id.source(),
        present: root.join(id.source()).exists(),
        reproduce: id.reproduce(),
        execution,
        digest_matches,
        proves,
    })
}

fn record(ctx: &Context, input: EvidenceRecordInput) -> Result<RecordReport, CapabilityError> {
    let root = root_of(ctx);
    let origin = match input.origin.as_deref() {
        None => Origin::Local,
        Some(w) => Origin::parse(w).ok_or_else(|| {
            CapabilityError::InvalidInput(format!(
                "`{w}` is not an origin: it is `local`, `ci` or `release`"
            ))
        })?,
    };
    let req = RecordRequest {
        suite: input.suite.map(std::path::PathBuf::from),
        crate_output: input.crate_output.map(std::path::PathBuf::from),
        origin,
    };
    let got = evidence::record(&root, &req).map_err(|e| match e {
        crate::error::Error::InvalidSurface { reason, .. } => CapabilityError::Refused(reason),
        other => internal(other),
    })?;
    Ok(RecordReport {
        recorded: got.recorded,
        passed: got.passed,
        commit: got.commit,
        working_tree: got.working_tree,
        unknown: got.unknown,
        ledger: evidence::LEDGER_PATH.to_string(),
    })
}

// ---------------------------------------------------------------- the module

/// The `evidence` module: claims joined to the runs recorded against them.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "evidence",
        title: "Evidence",
        description: "What actually ran, against which commit, and whether it still proves anything. The claims matrix binds a claim to a test by path; the ledger under .ai/repo/evidence records the latest execution of every test with the commit, the tree state, the digest of the test's own source, the time, the origin and the command that runs it again. Joining the two answers, per claim, whether the repository can honestly call it proven — and distinguishes a run recorded against this very commit from one whose inputs merely have not changed since, because collapsing those two is how a green badge stops meaning anything.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "evidence.report",
                title: "Every claim against the evidence recorded for it",
                description: "The whole claims matrix joined to the ledger: per claim, the proof state, the sentence explaining how that state was derived, the execution behind it, the files that changed since it, and the command that produces it again. The tallies count the whole matrix even when the answer is filtered, and the findings name every claim that declares a guarantee the evidence does not support. Read fresh on every call: the ledger is a file that changes outside this process.",
                input: EvidenceReportInput,
                output: EvidenceReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(crate::capability::model::McpExposure {
                        tool: Some("majordomus_evidence".into()),
                        resource: Some(McpResource { uri: EVIDENCE_URI.into(), name: "evidence".into() }),
                    }),
                    http: get("/api/v1/evidence"),
                    cli: Some(CliExposure { path: vec!["evidence".into(), "show".into()] }),
                },
                tags: ["evidence", "claims", "tests", "verification", "provenance"],
                cache: CachePolicy::Disabled,
                handler: report,
            },
            capability! {
                id: "evidence.claim",
                title: "What proves this claim",
                description: "One claim with its proof state, the execution behind it — outcome, duration, commit, tree state, digest, time and origin — the command that reproduces it, and the other claims the same test proves. A claim the matrix does not declare is a not-found rather than an empty answer, because a typo that read as 'this claim has no evidence' is the one answer this capability must never give.",
                input: EvidenceClaimInput,
                output: ClaimEvidence,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_evidence_claim"),
                    http: get("/api/v1/evidence/claim"),
                    cli: Some(CliExposure { path: vec!["evidence".into(), "claim".into()] }),
                },
                tags: ["evidence", "claims", "verification", "provenance"],
                cache: CachePolicy::Disabled,
                handler: claim,
            },
            capability! {
                id: "evidence.test",
                title: "What this test proves",
                description: "One test — by identity or by the path a claim names it with — with its latest execution, whether its source still hashes to what that execution recorded, the command that runs it again, and every claim that names it. This is the reverse of evidence.claim and reads the same derivation, so the two directions cannot disagree.",
                input: EvidenceTestInput,
                output: TestEvidence,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_evidence_test"),
                    http: get("/api/v1/evidence/test"),
                    cli: Some(CliExposure { path: vec!["evidence".into(), "proves".into()] }),
                },
                tags: ["evidence", "tests", "verification", "provenance"],
                cache: CachePolicy::Disabled,
                handler: test,
            },
            capability! {
                id: "evidence.record",
                kind: CapabilityKind::Command,
                title: "Record a run that happened",
                description: "Reads what the runs already wrote — the suite's TSV report, cargo test's output — stamps each result with the provenance the run itself did not carry (the commit, the tree state, the digest of the test's own source, the time, the origin) and merges it into the ledger. It records; it decides nothing: a case that failed is a case the runner said failed, and a test no report named is left exactly as it was, so recording one case never erases the evidence for the rest. A tree with no commit to name is refused, because an execution with no commit proves nothing.",
                input: EvidenceRecordInput,
                output: RecordReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["evidence".into(), "record".into()] }),
                },
                tags: ["evidence", "tests", "provenance"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::Destructive },
                handler: record,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; every projection derives from
    /// it. This is the assertion a refactor that dropped an exposure would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "evidence");
        let expected: &[(&str, Option<&str>, Option<&str>, &[&str])] = &[
            (
                "evidence.report",
                Some("majordomus_evidence"),
                Some("/api/v1/evidence"),
                &["evidence", "show"],
            ),
            (
                "evidence.claim",
                Some("majordomus_evidence_claim"),
                Some("/api/v1/evidence/claim"),
                &["evidence", "claim"],
            ),
            (
                "evidence.test",
                Some("majordomus_evidence_test"),
                Some("/api/v1/evidence/test"),
                &["evidence", "proves"],
            ),
            ("evidence.record", None, None, &["evidence", "record"]),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _, _)| *id).collect();
        assert_eq!(ids, want);
        for (e, (id, tool, path, cli)) in m.capabilities.iter().zip(expected) {
            let x = &e.capability.exposure;
            assert_eq!(x.mcp.as_ref().and_then(|m| m.tool.as_deref()), *tool, "{id}");
            assert_eq!(x.http.as_ref().map(|h| h.path.as_str()), *path, "{id}");
            assert_eq!(
                x.cli.as_ref().map(|c| c.path.clone()),
                Some(cli.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
                "{id} lost or renamed its command line"
            );
            assert!(
                !e.capability.cache.is_enabled(),
                "{id} must not cache: the ledger changes outside this process"
            );
        }
    }

    /// The recorder writes a tracked file. It must be a command, and it must be offered to
    /// whoever runs the executable and to nobody over a network — this server is read-only,
    /// and a write reachable over MCP or HTTP would end that.
    #[test]
    fn only_the_recorder_writes_and_it_is_not_reachable_over_the_network() {
        for e in module().capabilities {
            let id = e.capability.id.as_str().to_string();
            let x = &e.capability.exposure;
            if id == "evidence.record" {
                assert!(!e.capability.kind.is_read_only(), "{id} is declared a query");
                assert!(x.mcp.is_none(), "{id} is reachable over MCP");
                assert!(x.http.is_none(), "{id} is reachable over HTTP");
                assert_eq!(
                    e.capability.visibility,
                    crate::capability::Visibility::Developer,
                    "{id} must be a developer capability"
                );
                assert!(
                    matches!(e.capability.benchmark, BenchmarkPolicy::Waived { .. }),
                    "{id} must not be timed in a loop: it writes a tracked file"
                );
            } else {
                assert!(e.capability.kind.is_read_only(), "{id} writes");
                assert!(x.mcp.is_some() && x.http.is_some(), "{id} lost a projection");
            }
        }
    }

    /// Every read capability carries a command line, and every command line it carries is
    /// under one word. A capability that claimed a path `cli.rs` does not declare would
    /// fail the projection closure; this is the half that says the module meant to have one.
    #[test]
    fn every_capability_is_reachable_from_the_command_line() {
        for e in module().capabilities {
            let cli = e
                .capability
                .exposure
                .cli
                .as_ref()
                .unwrap_or_else(|| panic!("{} has no command line", e.capability.id));
            assert_eq!(
                cli.path.first().map(String::as_str),
                Some("evidence"),
                "{} is not under `majordomus evidence`",
                e.capability.id
            );
            assert_eq!(cli.path.len(), 2, "{}", e.capability.id);
        }
    }
}
