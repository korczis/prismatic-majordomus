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
//!
//! ```
//! use majordomus_cli::capability::builtin::evidence;
//!
//! let m = evidence::module();
//! assert_eq!(m.id.as_str(), "evidence");
//! let ids: Vec<&str> = m
//!     .capabilities
//!     .iter()
//!     .map(|e| e.capability.id.as_str())
//!     .collect();
//! assert_eq!(
//!     ids,
//!     [
//!         "evidence.report",
//!         "evidence.claim",
//!         "evidence.test",
//!         "evidence.record",
//!         "evidence.stamp",
//!         "evidence.subject",
//!         "evidence.coverage",
//!         "evidence.publish"
//!     ]
//! );
//!
//! // the recorder writes a tracked file, so it is offered to whoever runs the executable
//! // and to nobody over a network; that is what keeps the server read-only
//! let record = m
//!     .capabilities
//!     .iter()
//!     .find(|e| e.capability.id.as_str() == "evidence.record")
//!     .unwrap();
//! assert!(record.capability.exposure.mcp.is_none());
//! assert!(record.capability.exposure.http.is_none());
//! assert_eq!(
//!     record.capability.exposure.cli.as_ref().unwrap().path,
//!     ["evidence".to_string(), "record".to_string()]
//! );
//! ```
//!
//! # Why the stamp is offered only to whoever runs the executable
//!
//! `evidence.stamp` writes nothing, but it reads a file whose path its caller names (the
//! report it digests) and measures the checkout it runs in. A network client must not choose
//! a path this executable reads, so the stamp too declares a command line and nothing else.
//!
//! # Any subject, and the coverage beside it
//!
//! `evidence.subject` answers for every kind of the subject index
//! ([`crate::evidence::subject`]) — a claim, a rule, a feature, a command, a capability, an
//! MCP tool or a use case — through the one judge that index already has, so a feature's
//! verdict on the command line, over HTTP and over MCP is one derivation. `evidence.coverage`
//! lists the coverage records the run records carry. Coverage is supporting evidence: a
//! record reads `current` only for the checked-out commit, measured on a clean tree, with
//! the checkout clean as well, and it never decides a verdict (ADR 0087, decision 12). It
//! reads run records only from inside the checkout, because a network client must not
//! choose a file outside it for this executable to read.
//!
//! # The site's publication
//!
//! `evidence.publish` writes the site's evidence publication
//! ([`crate::evidence::publication`]): every claim judged from the ledger committed at the
//! presented commit, weakened through the monotone rule by the run records it is given, one
//! publication state from ADR 0087 decision 3's table, and only the fields of the allow-list,
//! each text through the one redactor. It reads a directory its caller names and writes a
//! file into another, outside the checkout, so like the recorder it is a command offered only
//! on the command line, and it is waived from the benchmark. It changes nothing in the
//! repository: an `--out` inside the checkout is refused.

//! # Example
//!
//! The capabilities, and every projection — the command line, the HTTP operation, the MCP
//! tool — is derived from that one declaration.
//!
//! ```
//! use majordomus_cli::capability::builtin::evidence::module;
//! let m = module();
//! assert_eq!(m.id.as_str(), "evidence");
//! assert_eq!(m.capabilities.len(), 8);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CachePolicy, CapabilityKind, CliExposure, Exposure, McpResource, Stability,
    WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::evidence::publication::{self, EvidencePublication, PublishedUnread, Run};
use crate::evidence::subject::{self, SubjectEvidence, SubjectKind};
use crate::evidence::{
    self, freshness, ClaimProof, EvidenceCoverage, EvidenceDropped, EvidenceProducer,
    EvidenceProvenance, EvidenceReport, EvidenceRunRecord, Execution, Ledger, LedgerTarget, Origin,
    Presented, ProofState, RecordRequest, StampRequest, TreeState, RUN_RECORD_SCHEMA,
};
use crate::{capability, module};

use super::{get, mcp};

/// The URI under which the whole evidence report is read as an MCP resource.
pub const EVIDENCE_URI: &str = "majordomus://evidence";

/// Where `evidence.coverage` reads run records when no source is named: the local run
/// directory ADR 0087 (decision 2) declares, inside the ignored local layer.
pub const RUNS_DIR: &str = ".ai/local/evidence/runs";

/// How a reader that named an unknown subject finds the ones there are.
const LIST_SUBJECTS: &str = "every subject is listed under `subjects` in \
site/data/registry/evidence-subjects.json, which `majordomus generate site` writes";

// ---------------------------------------------------------------- inputs

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which part of the matrix to answer for, and at which revision.
///
/// Every filter narrows, and they compose; saying nothing asks for the whole matrix at the
/// working tree. A key nothing recognises is refused rather than dropped, because a
/// misspelled filter that read as "no filter" would answer a question nobody asked and look
/// like a clean one.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::EvidenceReportInput;
/// use majordomus_cli::evidence::{ProofState, TreeState};
///
/// let all: EvidenceReportInput = serde_json::from_str("{}").unwrap();
/// assert!(all.state.is_none() && all.status.is_none() && !all.findings_only);
/// assert!(all.presented.is_none(), "the working tree unless a commit is named");
///
/// let narrowed: EvidenceReportInput =
///     serde_json::from_str(r#"{"state": "stale", "findings_only": true}"#).unwrap();
/// assert_eq!(narrowed.state, Some(ProofState::Stale));
/// assert!(narrowed.findings_only);
///
/// let at: EvidenceReportInput =
///     serde_json::from_str(r#"{"presented": "HEAD", "presented_tree": "dirty"}"#).unwrap();
/// assert_eq!(at.presented.as_deref(), Some("HEAD"));
/// assert_eq!(at.presented_tree, Some(TreeState::Dirty));
///
/// assert!(serde_json::from_str::<EvidenceReportInput>(r#"{"states": "stale"}"#).is_err());
/// ```
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Judge at this revision, as committed, rather than at the working tree: `HEAD`, or
    /// any name of the checked-out commit. It must be the checked-out commit; the ledger is
    /// read as that commit holds it, and a run the working ledger holds that the commit's
    /// does not can only withhold `proven`. Absent: the working tree.
    pub presented: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// What the caller knows about the presented commit's tree (`clean`, `dirty`,
    /// `unknown`). It can only weaken the measured state, never assert a clean tree over a
    /// dirty checkout, and it means nothing without `presented`.
    pub presented_tree: Option<TreeState>,
}

impl BenchmarkCases for EvidenceReportInput {
    /// The whole matrix and the findings, at the working tree. No case fills `presented`:
    /// the benchmark repository is not a git repository, so every revision it could name is
    /// refused, and such a case would time the refusal and be counted as coverage of the
    /// judgement it never reached.
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
/// One claim of the matrix, named by the id it is declared under.
///
/// The id is required, and there is no spelling of this input that means "every claim" —
/// that question belongs to `evidence.report`. An id the matrix does not declare is a
/// not-found from the handler rather than an empty proof.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::EvidenceClaimInput;
///
/// let input: EvidenceClaimInput =
///     serde_json::from_str(r#"{"claim": "evidence-both-directions"}"#).unwrap();
/// assert_eq!(input.claim, "evidence-both-directions");
///
/// // no id is a usage error, never a request for all of them
/// assert!(serde_json::from_str::<EvidenceClaimInput>("{}").is_err());
/// ```
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
///
/// Both spellings reach the same test. `docs/CLAIMS.yaml` names a path, the ledger keys on
/// an identity, and a caller holding one of them should not have to know the other; the
/// handler resolves a path to the identity and reads the same derivation either way.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::EvidenceTestInput;
/// use majordomus_cli::evidence::{Runner, TestId};
///
/// let by_path: EvidenceTestInput =
///     serde_json::from_str(r#"{"test": "test/cases/84_distribution_model.sh"}"#).unwrap();
/// let id = TestId::of(&by_path.test).unwrap();
/// assert_eq!(id.as_string(), "suite:84_distribution_model");
/// assert_eq!(id.runner, Runner::Suite);
///
/// // an identity is not a path: `TestId::of` reads paths, and the handler parses the
/// // `<runner>:<name>` form itself
/// let by_id: EvidenceTestInput = serde_json::from_str(r#"{"test": "crate:why"}"#).unwrap();
/// assert!(TestId::of(&by_id.test).is_none());
/// ```
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
///
/// Paths to what the runners already wrote, not a run to perform: the recorder reads a
/// report, carries into each result what that run measured about its own checkout, and
/// merges it. The reports are optional and may be given together; an absent origin is
/// `local`, and an absent ledger is the tracked one.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::EvidenceRecordInput;
///
/// let input: EvidenceRecordInput =
///     serde_json::from_str(r#"{"suite": "tmp/report.tsv"}"#).unwrap();
/// assert_eq!(input.suite.as_deref(), Some("tmp/report.tsv"));
/// assert!(input.crate_output.is_none() && input.origin.is_none());
///
/// // what was not given is not written back as null: the shape a client reads is the
/// // shape it sent
/// assert_eq!(
///     serde_json::to_string(&input).unwrap(),
///     r#"{"suite":"tmp/report.tsv"}"#
/// );
/// ```
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// A measurement per report: the file `majordomus evidence stamp` wrote at the end of
    /// that run, as `<file>` or `<producer>=<file>` (`suite`, `crate`, `coverage`). Without a
    /// producer word, the producer is the file's own, else the only report given.
    pub provenance: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A coverage summary: what `scripts/rust-coverage --summary-json` wrote.
    pub coverage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The ledger to merge into: `repo` (the default, the tracked ledger) or `local` (the
    /// ignored one under `.ai/local/`, which moves no verdict).
    pub ledger: Option<LedgerTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Also write the run record to this file.
    pub run_record: Option<String>,
}

impl BenchmarkCases for EvidenceRecordInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // nothing: the capability is waived from the benchmark because it writes a tracked
        // file, and timing it in a loop would rewrite the repository's evidence
        Vec::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// What a run asks to have measured about the checkout it leaves: its producer, its report,
/// and its own untracked outputs, which are not a change to what it tested.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::EvidenceStampInput;
/// use majordomus_cli::evidence::EvidenceProducer;
///
/// let input: EvidenceStampInput =
///     serde_json::from_str(r#"{"producer": "suite", "exclude": ["dist"]}"#).unwrap();
/// assert_eq!(input.producer, Some(EvidenceProducer::Suite));
/// assert_eq!(input.exclude, ["dist"]);
/// assert_eq!(serde_json::to_string(&input).unwrap(), r#"{"producer":"suite","exclude":["dist"]}"#);
/// ```
pub struct EvidenceStampInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The producer whose run this is: `suite`, `crate` or `coverage`.
    pub producer: Option<EvidenceProducer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The report the run wrote; a local recording needs it named.
    pub report: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The run's own untracked outputs, as repository paths: a file, or a directory with
    /// everything under it. A pattern, the whole checkout or a path holding tracked files is
    /// refused.
    pub exclude: Vec<String>,
}

impl BenchmarkCases for EvidenceStampInput {
    /// Nothing: the benchmark repository (`src/synthetic.rs`) is not a git work tree, so
    /// every case would time a refusal ("there is no commit to measure") rather than a
    /// measurement. The capability is waived as depending on an external `git`.
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// One subject of the evidence index, by its key, and the revision to judge it at.
///
/// The key is required: there is no spelling of this input that means "every subject". A key
/// the index does not hold is a not-found from the handler, never an empty verdict, because
/// "this feature has no evidence" read off a typo is the answer this capability must not give.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::EvidenceSubjectInput;
/// use majordomus_cli::evidence::TreeState;
///
/// let input: EvidenceSubjectInput =
///     serde_json::from_str(r#"{"subject": "feature:evidence"}"#).unwrap();
/// assert_eq!(input.subject, "feature:evidence");
/// assert!(input.presented.is_none(), "the working tree unless a commit is named");
///
/// let at: EvidenceSubjectInput = serde_json::from_str(
///     r#"{"subject": "command:evidence show", "presented": "HEAD", "presented_tree": "dirty"}"#,
/// )
/// .unwrap();
/// assert_eq!(at.presented_tree, Some(TreeState::Dirty));
///
/// // no key is a usage error, never a request for all of them
/// assert!(serde_json::from_str::<EvidenceSubjectInput>("{}").is_err());
/// ```
pub struct EvidenceSubjectInput {
    /// The subject's key, `<kind>:<id>`: `claim:<id>`, `rule:<id>`, `feature:<id>`,
    /// `command:<words>`, `capability:<id>`, `mcp:<tool>` or `use_case:<id>`.
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Judge at this revision, as committed, rather than at the working tree: `HEAD`, or any
    /// name of the checked-out commit, exactly as `evidence.report` reads it. Absent: the
    /// working tree.
    pub presented: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// What the caller knows about the presented commit's tree (`clean`, `dirty`,
    /// `unknown`). It can only weaken the measured state, and it means nothing without
    /// `presented`.
    pub presented_tree: Option<TreeState>,
}

impl BenchmarkCases for EvidenceSubjectInput {
    /// A claim this repository declares and a capability of this module, both at the working
    /// tree: the first measures the claim join through the index, the second a subject made
    /// of members. No case fills `presented`, for the reason `evidence.report` gives.
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let claim = ctx
            .index
            .objects
            .iter()
            .find(|o| o.kind == "claim")
            .map(|o| SubjectKind::Claim.key(&o.identity))
            .unwrap_or_else(|| SubjectKind::Claim.key("no-claim"));
        let at = |subject: String| EvidenceSubjectInput {
            subject,
            presented: None,
            presented_tree: None,
        };
        vec![
            NamedCase::new("first-claim", at(claim)),
            NamedCase::new(
                "capability",
                at(SubjectKind::Capability.key("evidence.report")),
            ),
        ]
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Where the run records are read from, and what the caller knows about the checkout.
///
/// Saying nothing reads the local run directory, [`RUNS_DIR`], at the checked-out commit as
/// measured.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::EvidenceCoverageInput;
/// use majordomus_cli::evidence::TreeState;
///
/// let all: EvidenceCoverageInput = serde_json::from_str("{}").unwrap();
/// assert!(all.from.is_none() && all.presented_tree.is_none());
///
/// let named: EvidenceCoverageInput =
///     serde_json::from_str(r#"{"from": "ci-evidence", "presented_tree": "dirty"}"#).unwrap();
/// assert_eq!(named.from.as_deref(), Some("ci-evidence"));
/// assert_eq!(named.presented_tree, Some(TreeState::Dirty));
/// assert!(serde_json::from_str::<EvidenceCoverageInput>(r#"{"form": "x"}"#).is_err());
/// ```
pub struct EvidenceCoverageInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A run record (what `evidence record --run-record` wrote, or a CI artifact's
    /// `record.json`), or a directory whose `*.json` files are read as run records. It must
    /// be inside the repository: relative to its root, or absolute under it. Absent: the
    /// local run directory, `.ai/local/evidence/runs`.
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// What the caller knows about the checked-out commit's tree (`clean`, `dirty`,
    /// `unknown`). It can only weaken the measured state: a site build that knows its tree
    /// was dirty shows no coverage as current.
    pub presented_tree: Option<TreeState>,
}

impl BenchmarkCases for EvidenceCoverageInput {
    /// The local run directory, and the same read with the presented tree weakened. Neither
    /// names a path: the benchmark repository has no run record to read, and a named path it
    /// lacks would time the refusal.
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("local-runs", EvidenceCoverageInput::default()),
            NamedCase::new(
                "presented-dirty",
                EvidenceCoverageInput {
                    from: None,
                    presented_tree: Some(TreeState::Dirty),
                },
            ),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// What to publish: the presented commit and what the caller knows of its tree, where the run
/// records are, what became of the presented commit's CI run, and where the publication goes.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::EvidencePublishInput;
/// use majordomus_cli::evidence::publication::Run;
/// use majordomus_cli::evidence::TreeState;
///
/// let input: EvidencePublishInput = serde_json::from_str(
///     r#"{"presented": "HEAD", "presented_tree": "clean", "from": "ci", "run": "concluded", "out": "/tmp/p"}"#,
/// )
/// .unwrap();
/// assert_eq!(input.presented_tree, Some(TreeState::Clean));
/// assert_eq!(input.run, Some(Run::Concluded));
///
/// // the commit and the destination are required: there is no default publication
/// assert!(serde_json::from_str::<EvidencePublishInput>(r#"{"out": "/tmp/p"}"#).is_err());
/// assert!(serde_json::from_str::<EvidencePublishInput>(r#"{"presented": "HEAD"}"#).is_err());
/// ```
pub struct EvidencePublishInput {
    /// The presented commit: `build.json.commit`, which must be the checked-out commit
    /// (`HEAD`, or any name of it). It is judged from the ledger it holds.
    pub presented: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// What the caller knows about the presented commit's tree (`clean`, `dirty`, `unknown`):
    /// `build.json.dirty`. It can only weaken the measured state.
    pub presented_tree: Option<TreeState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A run record, or a directory whose `*.json` files are read as run records: the CI
    /// evidence artifact of the presented commit or of its nearest ancestor. Absent: none is
    /// read.
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// What became of the presented commit's CI run: `pending`, `concluded` or `absent`.
    /// Absent: not known, which never reads `pending`.
    pub run: Option<Run>,
    /// The directory the publication is written into, as `evidence-publication.json`. It must
    /// be outside the checkout.
    pub out: String,
}

impl BenchmarkCases for EvidencePublishInput {
    /// Nothing: the capability writes a file and is waived from the benchmark, and the
    /// benchmark repository is not a git work tree with a commit to present.
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        Vec::new()
    }
}

// ---------------------------------------------------------------- outputs

#[derive(Debug, Clone, Serialize, JsonSchema)]
/// One claim with everything the repository can say about its proof, and the way back out
/// to the test that carries it.
///
/// The proof is flattened in, so a client reads one claim rather than an evidence wrapper
/// around one; the executions and the other claims the same test proves sit beside it.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::ClaimEvidence;
/// use majordomus_cli::evidence::ClaimProof;
///
/// let proof: ClaimProof = serde_json::from_value(serde_json::json!({
///     "id": "evidence-both-directions",
///     "claim": "a claim and its test answer for each other",
///     "status": "guaranteed",
///     "source": null,
///     "implementation": null,
///     "test_path": "test/cases/84_distribution_model.sh",
///     "test": "suite:84_distribution_model",
///     "state": "not_run",
///     "meaning": "no run of this test has been recorded",
///     "execution": null,
///     "changed": [],
///     "reproduce": "bash test/run.sh 84_distribution_model"
/// }))
/// .unwrap();
///
/// let answer = ClaimEvidence {
///     proof,
///     executions: Vec::new(),
///     also_proves: vec!["evidence-record-is-not-networked".to_string()],
/// };
/// let json = serde_json::to_value(&answer).unwrap();
/// assert_eq!(json["id"], "evidence-both-directions");
/// assert!(json.get("proof").is_none(), "the claim is flattened in, not nested");
/// assert_eq!(json["also_proves"], serde_json::json!(["evidence-record-is-not-networked"]));
/// // a list even where the ledger keeps one: retention is a decision that may change,
/// // and a client that read a bare object would break when it did
/// assert_eq!(json["executions"], serde_json::json!([]));
/// ```
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
///
/// The other direction of [`ClaimEvidence`], read from the test's end: its identity, its
/// source and whether that source is still the one that ran, the command that runs it
/// again, and every claim that names it.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::TestEvidence;
/// use majordomus_cli::evidence::TestId;
///
/// let id = TestId::of("apps/majordomus-cli/tests/why.rs").unwrap();
/// let answer = TestEvidence {
///     test: id.as_string(),
///     runner: id.runner,
///     source: id.source(),
///     present: true,
///     reproduce: id.reproduce(),
///     execution: None,
///     digest_matches: None,
///     proves: Vec::new(),
/// };
/// assert_eq!(answer.test, "crate:why");
/// assert_eq!(answer.reproduce, "cargo test --test why");
///
/// // nothing recorded: the two provenance keys are absent rather than null, so "no
/// // execution" cannot be read as an execution that said nothing
/// let json = serde_json::to_value(&answer).unwrap();
/// assert!(json.get("execution").is_none() && json.get("digest_matches").is_none());
/// assert_eq!(json["proves"], serde_json::json!([]));
/// ```
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
/// What a recording did: how much of the run reached the ledger, and under what provenance.
///
/// The recorder decides nothing, and this report says so. A result no runner in this
/// repository owns is named in `unknown` rather than written under a guessed identity, and
/// a failure is recorded like a pass — the ledger holds what happened, so `passed` is a
/// count and never a filter.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::RecordReport;
///
/// let report = RecordReport {
///     recorded: 3,
///     passed: 2,
///     commit: "06fa258913a1b2c3d4e5f60718293a4b5c6d7e8f".to_string(),
///     working_tree: "clean".to_string(),
///     unknown: vec!["docs/CLAIMS.yaml".to_string()],
///     dropped: vec![],
///     ledger: majordomus_cli::evidence::LEDGER_PATH.to_string(),
///     run_record: serde_json::from_value(serde_json::json!({
///         "schema": 1, "id": "local:06fa258913a1:20260926T120000Z", "origin": "local",
///         "commit": "06fa258913a1b2c3d4e5f60718293a4b5c6d7e8f", "working_tree": "clean",
///         "recorded_at": "2026-09-26T12:00:00Z", "ledger": "repo"
///     }))
///     .unwrap(),
/// };
/// // the failing test is one of the three recorded, not a fourth thing that was dropped
/// assert_eq!(report.recorded - report.passed, 1);
/// assert!(report.ledger.starts_with(".ai/repo/"));
///
/// let json = serde_json::to_value(&report).unwrap();
/// assert_eq!(json["unknown"].as_array().unwrap().len(), 1);
/// assert_eq!(json["working_tree"], "clean");
/// // a suite-only recording listed nothing, and its document is what it always was
/// assert!(json.get("dropped").is_none());
/// ```
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
    /// What the reports held that no claim can name yet: the crate's unit-test binary, its
    /// doctests. Listed rather than ignored, and absent from the document when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dropped: Vec<EvidenceDropped>,
    /// Where the ledger was written: the path of the target the recording named.
    pub ledger: String,
    /// The recording as a typed run.
    pub run_record: EvidenceRunRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Whether a coverage record describes the checkout presented (ADR 0087, decision 12).
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::EvidenceCoverageState;
/// assert_eq!(serde_json::to_value(EvidenceCoverageState::Stale).unwrap(), "stale");
/// ```
pub enum EvidenceCoverageState {
    /// Measured at the presented commit on a clean tree, and the presented tree is clean.
    Current,
    /// Anything else: shown with its commit, and never as a measurement of this checkout.
    Stale,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
/// The checkout a coverage record is judged against: the checked-out commit and its tree,
/// measured ignoring the evidence ledger's working copy and weakened by what the caller knew.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::EvidenceCoveragePresented;
/// use majordomus_cli::evidence::TreeState;
///
/// let none = EvidenceCoveragePresented {
///     commit: None,
///     tree: TreeState::Unknown,
///     reason: Some("this repository has no checked-out commit".into()),
/// };
/// let json = serde_json::to_value(&none).unwrap();
/// assert!(json["commit"].is_null(), "no commit is a stated absence, never a guess");
/// assert_eq!(json["tree"], "unknown");
/// ```
pub struct EvidenceCoveragePresented {
    /// The full id of the checked-out commit, or `null` when git cannot name one.
    pub commit: Option<String>,
    /// That commit's tree as measured, weakened by `presented_tree`; `unknown` without a
    /// commit.
    pub tree: TreeState,
    /// Why no commit is presented, when none is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
/// One coverage record, as one run record carries it, judged against the presented checkout.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::{EvidenceCoverageRecord, EvidenceCoverageState};
/// use majordomus_cli::evidence::{CommitId, EvidenceCoverage, Origin, TreeState};
///
/// let one = r#"{"covered":1,"total":2,"percent":50.0}"#;
/// let dims = format!(r#"{{"lines":{one},"functions":{one},"regions":{one}}}"#);
/// let text = format!(
///     r#"{{"schema":1,"measurement":"m","test_code":"excluded","crate":{dims},
///         "domain":{{"lines":{one},"functions":{one},"regions":{one},"files":[],"missing":[]}},
///         "files":{{}}}}"#
/// );
/// let commit = CommitId::parse(&"c".repeat(40)).unwrap();
/// let coverage =
///     EvidenceCoverage::from_summary(&text, commit, TreeState::Clean, Default::default()).unwrap();
/// let record = EvidenceCoverageRecord {
///     run: "local:cccccccccccc:20261022T120000Z".into(),
///     origin: Origin::Local,
///     recorded_at: "2026-10-22T12:00:00Z".into(),
///     file: ".ai/local/evidence/runs/one.json".into(),
///     state: EvidenceCoverageState::Stale,
///     reason: "the presented tree is dirty".into(),
///     below_floor: Vec::new(),
///     coverage,
/// };
/// let json = serde_json::to_value(&record).unwrap();
/// assert_eq!(json["state"], "stale");
/// assert_eq!(json["coverage"]["commit"], "c".repeat(40), "shown with its own commit");
/// ```
pub struct EvidenceCoverageRecord {
    /// The id of the run record that carries it.
    pub run: String,
    /// Where that run happened.
    pub origin: Origin,
    /// When it was recorded, RFC 3339 UTC.
    pub recorded_at: String,
    /// The file it was read from, repository-relative.
    pub file: String,
    /// Whether it describes the presented checkout.
    pub state: EvidenceCoverageState,
    /// Why, in one sentence.
    pub reason: String,
    /// Which of the crate and the domain are below the line floor recorded with it.
    pub below_floor: Vec<String>,
    /// The measurement itself, with its own commit and tree.
    pub coverage: EvidenceCoverage,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
/// A file of the source that was not read as a run record, and why: listed rather than
/// skipped, so that a directory that held nothing readable never reads as one that held no
/// coverage.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::EvidenceCoverageUnread;
/// let u = EvidenceCoverageUnread { file: "manifest.json".into(), reason: "not a run record".into() };
/// assert_eq!(serde_json::to_value(&u).unwrap()["file"], "manifest.json");
/// ```
pub struct EvidenceCoverageUnread {
    /// The file, repository-relative.
    pub file: String,
    /// Why it was not read.
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
/// Every coverage record of the run records read, each judged against the presented checkout.
///
/// Coverage is supporting evidence: nothing here moves a verdict. A run record without a
/// coverage measurement is counted in `runs` and lists nothing.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::{
///     EvidenceCoverageAnswer, EvidenceCoveragePresented,
/// };
/// use majordomus_cli::evidence::TreeState;
///
/// let answer = EvidenceCoverageAnswer {
///     presented: EvidenceCoveragePresented { commit: Some("c".repeat(40)), tree: TreeState::Clean, reason: None },
///     source: ".ai/local/evidence/runs".into(),
///     present: false,
///     runs: 0,
///     records: Vec::new(),
///     unread: Vec::new(),
/// };
/// let json = serde_json::to_value(&answer).unwrap();
/// assert_eq!(json["present"], false, "a source that does not exist says so");
/// assert_eq!(json["records"], serde_json::json!([]));
/// ```
pub struct EvidenceCoverageAnswer {
    /// The checkout every record is judged against.
    pub presented: EvidenceCoveragePresented,
    /// The file or directory read, repository-relative.
    pub source: String,
    /// Whether it exists. The default local run directory may not, before anything wrote it.
    pub present: bool,
    /// How many run records were read, with or without coverage.
    pub runs: usize,
    /// The coverage records, in the order of the files that carry them.
    pub records: Vec<EvidenceCoverageRecord>,
    /// The files of the source that are not run records.
    pub unread: Vec<EvidenceCoverageUnread>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
/// A publication written: the file, and the publication it holds.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::EvidencePublished;
/// use majordomus_cli::evidence::freshness::TreeState;
/// use majordomus_cli::evidence::publication::{
///     EvidencePublication, PublicationState, PublishedLedger, PublishedPresented,
/// };
/// let written = EvidencePublished {
///     file: "out/evidence-publication.json".into(),
///     publication: EvidencePublication {
///         schema: 1,
///         presented: PublishedPresented { commit: "c".repeat(40), tree: TreeState::Clean },
///         state: PublicationState::Unknown,
///         reasons: vec!["no CI run of the presented commit is found".into()],
///         run: None,
///         ledger: PublishedLedger { path: ".ai/repo/evidence/ledger.json".into(), executions: 0, newest: None },
///         records: vec![], capped_by: vec![], claims: vec![], totals: Default::default(),
///         unread: vec![], withheld: 0,
///     },
/// };
/// assert_eq!(serde_json::to_value(&written).unwrap()["publication"]["state"], "unknown");
/// ```
pub struct EvidencePublished {
    /// The file written.
    pub file: String,
    /// What it holds, exactly as written.
    pub publication: EvidencePublication,
}

// ---------------------------------------------------------------- handlers

fn root_of(ctx: &Context) -> std::path::PathBuf {
    std::path::PathBuf::from(&ctx.index.repository.root)
}

fn internal(e: crate::error::Error) -> CapabilityError {
    CapabilityError::Internal(e.to_string())
}

/// The whole joined report, read fresh: the ledger is a file that changes outside this
/// process, and an answer kept past a change of it would be the stale evidence this module
/// exists to name. The executor keeps the capability's answer per checkout state only
/// (`CachePolicy::Checkout`), which an edit of the ledger moves.
fn full_report(ctx: &Context) -> Result<EvidenceReport, CapabilityError> {
    let root = root_of(ctx);
    let ledger = Ledger::load(&root).map_err(internal)?;
    Ok(evidence::report(&ctx.index, &ledger))
}

/// The joined report at the checked-out commit, as committed: the ledger that commit holds,
/// and the runs the working ledger holds beside it, which can only withhold `proven`.
fn report_at_commit(
    ctx: &Context,
    rev: &str,
    given: Option<TreeState>,
) -> Result<EvidenceReport, CapabilityError> {
    let (ledger, held, presented) = committed(ctx, rev, given)?;
    Ok(evidence::report_at(&ctx.index, &ledger, &presented, &held))
}

/// What a presented commit is judged from: the ledger it holds, the executions the working
/// ledger holds that it does not, and the commit with its measured tree. One reading, shared
/// by the claim report and the subject answer, so that the two cannot judge a commit apart.
fn committed(
    ctx: &Context,
    rev: &str,
    given: Option<TreeState>,
) -> Result<(Ledger, Vec<Execution>, Presented), CapabilityError> {
    let root = root_of(ctx);
    let presented =
        freshness::presented_commit(&root, rev, given).map_err(CapabilityError::InvalidInput)?;
    let commit = presented.commit().unwrap_or_default().to_string();
    let ledger = freshness::ledger_at(&root, &commit).map_err(internal)?;
    let held = freshness::uncommitted(&root, &ledger).map_err(internal)?;
    Ok((ledger, held, presented))
}

/// A tree state with no commit to weaken is refused rather than ignored.
fn tree_without_commit() -> CapabilityError {
    CapabilityError::InvalidInput(
        "`presented_tree` weakens the measured tree of a presented commit, and no commit was \
         presented: name one with `presented`"
            .to_string(),
    )
}

fn report(ctx: &Context, input: EvidenceReportInput) -> Result<EvidenceReport, CapabilityError> {
    let mut r = match (&input.presented, input.presented_tree) {
        (Some(rev), given) => report_at_commit(ctx, rev, given)?,
        (None, Some(_)) => return Err(tree_without_commit()),
        (None, None) => full_report(ctx)?,
    };
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
            // the same rule the path form applies: one grammar, not two that disagree
            evidence::TestId::named(runner, name)
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
    // A CI run names itself from its environment; a local run has no run to name, and a
    // local recording inside a CI job's shell is still not that job's evidence.
    let run = match origin {
        Origin::Ci => evidence::RunRef::from_env(|k| std::env::var(k).ok()),
        Origin::Local | Origin::Release => None,
    };
    let ledger = input.ledger.unwrap_or_default();
    let given: Vec<EvidenceProducer> = [
        (EvidenceProducer::Suite, input.suite.is_some()),
        (EvidenceProducer::Crate, input.crate_output.is_some()),
        (EvidenceProducer::Coverage, input.coverage.is_some()),
    ]
    .into_iter()
    .filter_map(|(p, is)| is.then_some(p))
    .collect();
    let provenance =
        EvidenceProvenance::resolve(&input.provenance, &given).map_err(CapabilityError::Refused)?;
    let req = RecordRequest {
        suite: input.suite.map(std::path::PathBuf::from),
        crate_output: input.crate_output.map(std::path::PathBuf::from),
        run,
        provenance,
        coverage: input.coverage.map(std::path::PathBuf::from),
        ledger,
        run_record: input.run_record.map(std::path::PathBuf::from),
        ..RecordRequest::new(origin)
    };
    let got = evidence::record(&root, &req).map_err(refused_or_internal)?;
    Ok(RecordReport {
        recorded: got.recorded,
        passed: got.passed,
        commit: got.commit,
        working_tree: got.working_tree,
        unknown: got.unknown,
        dropped: got.dropped,
        ledger: ledger.path().to_string(),
        run_record: got.run_record,
    })
}

/// A refusal of the evidence surface is the caller's to fix; anything else is this
/// executable's.
fn refused_or_internal(e: crate::error::Error) -> CapabilityError {
    match e {
        crate::error::Error::InvalidSurface { reason, .. } => CapabilityError::Refused(reason),
        other => internal(other),
    }
}

fn stamp(ctx: &Context, input: EvidenceStampInput) -> Result<EvidenceProvenance, CapabilityError> {
    let req = StampRequest {
        producer: input.producer,
        report: input.report.map(std::path::PathBuf::from),
        exclude: input.exclude,
        run: evidence::RunRef::from_env(|k| std::env::var(k).ok()),
    };
    evidence::stamp(&root_of(ctx), &req).map_err(refused_or_internal)
}

/// One subject, judged by the subject index's own judge: at the working tree from the
/// working ledger, or at a presented commit exactly as `evidence.report` reads it. An unknown
/// key is a not-found that says where the keys are listed.
fn subject_of(
    ctx: &Context,
    input: EvidenceSubjectInput,
) -> Result<SubjectEvidence, CapabilityError> {
    let (ledger, held, presented) = match (&input.presented, input.presented_tree) {
        (Some(rev), given) => committed(ctx, rev, given)?,
        (None, Some(_)) => return Err(tree_without_commit()),
        (None, None) => (
            Ledger::load(&root_of(ctx)).map_err(internal)?,
            Vec::new(),
            Presented::WorkingTree,
        ),
    };
    subject::answer(
        &ctx.index,
        &ctx.product,
        &ctx.registry,
        &ledger,
        &held,
        &presented,
        &input.subject,
    )
    .map_err(|unknown| CapabilityError::NotFound(format!("{unknown}; {LIST_SUBJECTS}")))
}

/// Every coverage record of the run records at the source, each judged against the
/// checked-out commit and its measured tree.
fn coverage(
    ctx: &Context,
    input: EvidenceCoverageInput,
) -> Result<EvidenceCoverageAnswer, CapabilityError> {
    let given = root_of(ctx);
    // the root as the filesystem resolves it, so that a named source is compared with the
    // same spelling of the checkout it must be inside
    let root = given.canonicalize().unwrap_or(given);
    let source = match &input.from {
        Some(from) => inside(&root, from)?,
        None => root.join(RUNS_DIR),
    };
    let presented = match freshness::presented_commit(&root, "HEAD", input.presented_tree) {
        Ok(p) => EvidenceCoveragePresented {
            commit: p.commit().map(str::to_string),
            tree: p.tree(),
            reason: None,
        },
        Err(reason) => EvidenceCoveragePresented {
            commit: None,
            tree: TreeState::Unknown,
            reason: Some(reason),
        },
    };
    let mut answer = EvidenceCoverageAnswer {
        source: relative(&root, &source),
        present: source.exists(),
        presented,
        runs: 0,
        records: Vec::new(),
        unread: Vec::new(),
    };
    for file in run_files(&source) {
        let name = relative(&root, &file);
        let run = match read_run(&file) {
            Ok(run) => run,
            Err(reason) => {
                answer
                    .unread
                    .push(EvidenceCoverageUnread { file: name, reason });
                continue;
            }
        };
        answer.runs += 1;
        let Some(coverage) = run.coverage else {
            continue;
        };
        let (state, reason) = coverage_state(&coverage, &answer.presented);
        answer.records.push(EvidenceCoverageRecord {
            run: run.id,
            origin: run.origin,
            recorded_at: run.recorded_at,
            file: name,
            state,
            reason,
            below_floor: coverage
                .below_floor()
                .into_iter()
                .map(str::to_string)
                .collect(),
            coverage,
        });
    }
    Ok(answer)
}

/// The source a caller named, resolved inside the checkout `root` (already canonical). A
/// path that does not exist is a not-found; one that resolves outside the checkout, through
/// `..`, an absolute path or a link, is refused, because a network client must not choose a
/// file outside the repository for this executable to read.
fn inside(root: &std::path::Path, from: &str) -> Result<std::path::PathBuf, CapabilityError> {
    let real = root
        .join(from)
        .canonicalize()
        .map_err(|e| CapabilityError::NotFound(format!("`{from}` cannot be read: {e}")))?;
    if !real.starts_with(root) {
        return Err(CapabilityError::Refused(format!(
            "`{from}` is outside the repository: run records are read only from inside the \
             checkout, because a network client must not choose a file outside it for this \
             executable to read; copy a CI evidence artifact under {RUNS_DIR} first"
        )));
    }
    Ok(real)
}

/// The files a source holds: itself when it is a file, else its `*.json` files in the
/// canonical order of their paths, not descending into subdirectories. A source that does
/// not exist holds none.
fn run_files(source: &std::path::Path) -> Vec<std::path::PathBuf> {
    if source.is_file() {
        return vec![source.to_path_buf()];
    }
    let mut paths: Vec<String> = std::fs::read_dir(source)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "json"))
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    crate::order::canonical_strings(&mut paths);
    paths.into_iter().map(std::path::PathBuf::from).collect()
}

/// One run record: a document `evidence record --run-record` wrote, or the recording's own
/// answer that carries one under `run_record`, as a CI evidence artifact's `record.json` is.
/// Anything else is named with the reason it is not one.
fn read_run(file: &std::path::Path) -> Result<EvidenceRunRecord, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("cannot be read: {e}"))?;
    let doc: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("not JSON: {e}"))?;
    let doc = doc.get("run_record").cloned().unwrap_or(doc);
    let run: EvidenceRunRecord =
        serde_json::from_value(doc).map_err(|e| format!("not a run record: {e}"))?;
    if run.schema != RUN_RECORD_SCHEMA {
        return Err(format!(
            "a run record of schema {}, which this reader does not read (schema \
             {RUN_RECORD_SCHEMA})",
            run.schema
        ));
    }
    Ok(run)
}

/// A path as the answer prints it: relative to the checkout when it is inside it.
fn relative(root: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// Whether one coverage record describes the presented checkout, and why: current only for
/// its own commit, measured clean, with the presented tree clean (ADR 0087, decision 12).
fn coverage_state(
    coverage: &EvidenceCoverage,
    presented: &EvidenceCoveragePresented,
) -> (EvidenceCoverageState, String) {
    let twelve = |c: &str| c.chars().take(12).collect::<String>();
    let Some(commit) = presented.commit.as_deref() else {
        return (
            EvidenceCoverageState::Stale,
            format!(
                "no commit is presented ({}), so no measurement can describe it",
                presented.reason.as_deref().unwrap_or("git named none")
            ),
        );
    };
    if coverage.is_current_for(commit, presented.tree) {
        return (
            EvidenceCoverageState::Current,
            format!(
                "measured at the presented commit {} on a clean tree, and the presented tree \
                 is clean",
                twelve(commit)
            ),
        );
    }
    let mut why = Vec::new();
    if coverage.commit.as_str() != commit {
        why.push(format!(
            "measured at {}, not at the presented commit {}",
            twelve(coverage.commit.as_str()),
            twelve(commit)
        ));
    }
    if coverage.working_tree != TreeState::Clean {
        why.push(format!(
            "its own run measured its tree as {}",
            coverage.working_tree.as_str()
        ));
    }
    if presented.tree != TreeState::Clean {
        why.push(format!("the presented tree is {}", presented.tree.as_str()));
    }
    (EvidenceCoverageState::Stale, why.join("; "))
}

/// The publication of the presented commit, written into a directory outside the checkout:
/// the claims judged from the ledger that commit holds, exactly as `evidence.report` judges
/// it, then the run records read from the source through the monotone rule, then the
/// allow-list and the redactor ([`crate::evidence::publication`]).
fn publish(
    ctx: &Context,
    input: EvidencePublishInput,
) -> Result<EvidencePublished, CapabilityError> {
    let given = root_of(ctx);
    let root = given.canonicalize().unwrap_or(given);
    let out = outside(&root, &input.out)?;
    let (ledger, held, presented) = committed(ctx, &input.presented, input.presented_tree)?;
    let report = evidence::report_at(&ctx.index, &ledger, &presented, &held);
    let mut records = Vec::new();
    let mut unread = Vec::new();
    if let Some(from) = &input.from {
        let source = root.join(from);
        if !source.exists() {
            return Err(CapabilityError::NotFound(format!(
                "`{from}` cannot be read: there is no such file or directory"
            )));
        }
        for file in run_files(&source) {
            let name = file
                .strip_prefix(&source)
                .ok()
                .filter(|p| !p.as_os_str().is_empty())
                .or(file.file_name().map(std::path::Path::new))
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            match read_run(&file) {
                Ok(run) => records.push(run),
                Err(reason) => unread.push(PublishedUnread { file: name, reason }),
            }
        }
    }
    let p = publication::publication(&root, &report, &presented, &records, unread, input.run)
        .map_err(CapabilityError::Refused)?;
    let text = publication::render(&p).map_err(CapabilityError::Refused)?;
    let file = out.join(publication::FILE);
    std::fs::create_dir_all(&out)
        .and_then(|()| std::fs::write(&file, text))
        .map_err(|e| {
            CapabilityError::Internal(format!("{} could not be written: {e}", file.display()))
        })?;
    Ok(EvidencePublished {
        file: file.to_string_lossy().into_owned(),
        publication: p,
    })
}

/// The directory a publication is written into, refused when it is inside the checkout
/// `root` (already canonical): the publication changes nothing in the repository. A path
/// that does not exist yet is resolved through its nearest existing ancestor, so that a link
/// cannot carry it inside.
fn outside(root: &std::path::Path, out: &str) -> Result<std::path::PathBuf, CapabilityError> {
    let mut lexical = std::path::PathBuf::new();
    for part in root.join(out).components() {
        match part {
            std::path::Component::ParentDir => {
                lexical.pop();
            }
            std::path::Component::CurDir => {}
            other => lexical.push(other),
        }
    }
    let mut existing = lexical.as_path();
    let mut rest = Vec::new();
    while !existing.exists() {
        rest.push(existing.file_name().unwrap_or_default().to_os_string());
        existing = existing.parent().unwrap_or(root);
    }
    let mut real = existing.canonicalize().unwrap_or(existing.to_path_buf());
    real.extend(rest.iter().rev());
    if real.starts_with(root) {
        return Err(CapabilityError::Refused(format!(
            "`{out}` is inside the checkout: the publication is written outside it, so that \
             publishing changes nothing in the repository; name a directory outside it"
        )));
    }
    if real.is_file() {
        return Err(CapabilityError::Refused(format!(
            "`{out}` is a file: the publication is written into a directory"
        )));
    }
    Ok(real)
}

// ---------------------------------------------------------------- the module

/// The `evidence` module: claims joined to the runs recorded against them.
///
/// The declaration is the whole of what the capabilities are — every projection, from
/// the HTTP route to the MCP tool to the word a person types, is read out of what it
/// returns. Nothing here caches: the ledger is a file that changes outside this process,
/// and a cached answer would be exactly the stale evidence the module exists to name.
///
/// ```
/// use majordomus_cli::capability::builtin::evidence::module;
/// use majordomus_cli::capability::CachePolicy;
///
/// let m = module();
/// let report = m
///     .capabilities
///     .iter()
///     .find(|e| e.capability.id.as_str() == "evidence.report")
///     .unwrap();
/// assert_eq!(report.capability.exposure.http.as_ref().unwrap().path, "/api/v1/evidence");
/// assert_eq!(
///     report.capability.exposure.mcp.as_ref().unwrap().tool.as_deref(),
///     Some("majordomus_evidence")
/// );
///
/// // `evidence.test` is spelled `majordomus evidence proves`: `test` is a word the fish
/// // completion adapter refuses, and the identity is not the command
/// let test = m
///     .capabilities
///     .iter()
///     .find(|e| e.capability.id.as_str() == "evidence.test")
///     .unwrap();
/// assert_eq!(test.capability.exposure.cli.as_ref().unwrap().path[1], "proves");
///
/// // the ledger changes outside this process: kept, if at all, per checkout state only
/// assert!(m.capabilities.iter().all(|e| matches!(
///     e.capability.cache,
///     CachePolicy::Disabled | CachePolicy::Checkout { .. }
/// )));
/// ```
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
                description: "The whole claims matrix joined to the ledger and judged at the presented revision: the working tree by default, or the checked-out commit as committed, read from the ledger that commit holds. Per claim: the proof state, the sentence explaining how that state was derived, why when the state alone does not say, the execution behind it, the files that changed since it, and the command that produces it again. A run on a commit the presented revision does not contain is stale, a test that declined to run is not run, and a run the working ledger holds that the presented commit's does not can only withhold proven. The tallies count the whole matrix even when the answer is filtered, and the findings name every claim that declares a guarantee the evidence does not support. Kept per checkout state, never longer: the ledger is a file that changes outside this process, and an edit of it, of a test or of any tracked file, a new untracked file, a commit or a new ref is a new answer.",
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
                cache: CachePolicy::Checkout { max_entries: 8 },
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
                cache: CachePolicy::Checkout { max_entries: 32 },
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
                description: "Reads what the runs already wrote — the suite's TSV report, cargo test's output, a coverage summary — carries into each result what that run measured about its own checkout (the commit and the tree, from the measurement `evidence stamp` wrote), with the digest of the test's own source, the time and the origin, and merges it into the ledger. It records; it decides nothing: a case that failed is a case the runner said failed, and a test no report named is left exactly as it was, so recording one case never erases the evidence for the rest. It refuses a tree with no commit, a measurement of another commit or of another report, a local measurement that names no report, and a CI report without a measurement; a local report recorded without one carries the tree unknown, which never reads proven. Every recording returns a run record (its commit, the tree each report's run measured, its totals and what was absent, dropped or unknown, and a coverage summary bound to its commit), and --ledger local writes only the ignored local ledger. A crate binary that ran no test, or only a filtered subset, is recorded as a skip, and what no claim can name yet (the crate's own unit tests, its doctests) is listed as dropped.",
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
            capability! {
                id: "evidence.stamp",
                title: "Measure the tree a run left behind",
                description: "The one measurement a runner takes at the end of its run, for evidence record --provenance: the commit; the tree, ignoring the evidence ledger and the run's own untracked outputs, never a tracked change; the producer; its toolchain; the recorder's version; the report's digest; the host; the CI run. It writes nothing. It is offered only on the command line because it reads a path its caller names.",
                input: EvidenceStampInput,
                output: EvidenceProvenance,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["evidence".into(), "stamp".into()] }),
                },
                tags: ["evidence", "tests", "provenance"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: stamp,
            },
            capability! {
                id: "evidence.subject",
                title: "What proves this subject",
                description: "One subject of the evidence index — a claim, a rule, a feature, a command, a capability, an MCP tool or a use case, named <kind>:<id> — judged at the presented revision: the working tree by default, or the checked-out commit as committed, read from the ledger that commit holds exactly as evidence.report reads it. Its verdict is the one aggregation over its own routes and its members' own verdicts: a failing part makes it failing, otherwise the weakest part that can carry proof decides, and the totals by state are always beside it. Each route carries its state, the execution behind it and the command that runs it again; each member carries its own verdict; a rule's paths no runner drives are listed as mechanisms, and the subject's advisory findings sit beside the verdict. A key the index does not hold is a not-found that says where the keys are listed, never an empty verdict. Kept per checkout state, never longer.",
                input: EvidenceSubjectInput,
                output: SubjectEvidence,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_evidence_subject"),
                    http: get("/api/v1/evidence/subject"),
                    cli: Some(CliExposure { path: vec!["evidence".into(), "subject".into()] }),
                },
                tags: ["evidence", "subjects", "verification", "provenance"],
                cache: CachePolicy::Checkout { max_entries: 32 },
                handler: subject_of,
            },
            capability! {
                id: "evidence.coverage",
                title: "The coverage the run records carry",
                description: "Every coverage record carried by the run records read — from the local run directory .ai/local/evidence/runs by default, or from a run record or a directory of them (a CI evidence artifact) named inside the checkout — each shown with its run, its own commit and tree, the floors recorded with it, and its state against the checked-out commit as measured: current only for its own commit, measured on a clean tree, with the checkout clean as well, and otherwise stale, with the reason. Coverage is supporting evidence and decides no verdict. A file of the source that is not a run record is listed with the reason rather than skipped, and a run record without coverage is counted. A source outside the checkout is refused, because a network client must not choose a file outside it for this executable to read.",
                input: EvidenceCoverageInput,
                output: EvidenceCoverageAnswer,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_evidence_coverage"),
                    http: get("/api/v1/evidence/coverage"),
                    cli: Some(CliExposure { path: vec!["evidence".into(), "coverage".into()] }),
                },
                tags: ["evidence", "coverage", "verification", "provenance"],
                cache: CachePolicy::Disabled,
                handler: coverage,
            },
            capability! {
                id: "evidence.publish",
                kind: CapabilityKind::Command,
                title: "Write the site's evidence publication",
                description: "Writes the site's evidence publication of the presented commit, which must be the checked-out commit, into a directory outside the checkout, as evidence-publication.json. Every claim is judged from the ledger that commit holds, exactly as evidence show --presented judges it, and the run records read from --from (the commit's own CI evidence artifact, or its nearest ancestor's) can only weaken a verdict through the monotone rule, never strengthen one. The publication carries one state, the first matching row of ADR 0087 decision 3's table: dirty when the presented tree is not clean, stale when a record read caps a verdict, current when the commit's own run record is read and every tree it carries was measured clean, pending when it is not read and the commit's CI run has not concluded, and unknown otherwise, naming which. Whatever row matched, every record that capped a verdict is named. Only the fields of the declared allow-list are published, every text passes the one redactor (a text it refuses is withheld and counted), and the artifact passes the generator's refusal of machine paths and credential markers. No state blocks publishing. An --out inside the checkout is refused, so publishing changes nothing in the repository.",
                input: EvidencePublishInput,
                output: EvidencePublished,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["evidence".into(), "publish".into()] }),
                },
                tags: ["evidence", "publication", "verification", "provenance"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::Destructive },
                handler: publish,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One capability's declared projections: its id, the MCP tool it answers as, the HTTP
    /// path it is served at, and the words of its command line. Named because the tuple is
    /// read four times below and an anonymous one of this width is a type nobody can talk
    /// about.
    type Projection<'a> = (&'a str, Option<&'a str>, Option<&'a str>, &'a [&'a str]);

    /// The declaration is the only place these names exist; every projection derives from
    /// it. This is the assertion a refactor that dropped an exposure would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "evidence");
        let expected: &[Projection<'_>] = &[
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
            ("evidence.stamp", None, None, &["evidence", "stamp"]),
            (
                "evidence.subject",
                Some("majordomus_evidence_subject"),
                Some("/api/v1/evidence/subject"),
                &["evidence", "subject"],
            ),
            (
                "evidence.coverage",
                Some("majordomus_evidence_coverage"),
                Some("/api/v1/evidence/coverage"),
                &["evidence", "coverage"],
            ),
            ("evidence.publish", None, None, &["evidence", "publish"]),
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
            assert_eq!(
                x.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                *tool,
                "{id}"
            );
            assert_eq!(x.http.as_ref().map(|h| h.path.as_str()), *path, "{id}");
            assert_eq!(
                x.cli.as_ref().map(|c| c.path.clone()),
                Some(cli.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
                "{id} lost or renamed its command line"
            );
            // the ledger changes outside this process, so an answer is kept, if at all, only
            // per checkout state, which an edit of the ledger moves
            assert!(
                matches!(
                    e.capability.cache,
                    CachePolicy::Disabled | CachePolicy::Checkout { .. }
                ),
                "{id} may cache only per checkout state: the ledger changes outside this process"
            );
        }
    }

    /// The recorder writes a tracked file and the publisher writes a file outside the
    /// checkout. Each must be a command, and each must be offered to whoever runs the
    /// executable and to nobody over a network — this server is read-only, and a write
    /// reachable over MCP or HTTP would end that.
    #[test]
    fn only_the_recorder_and_the_publisher_write_and_neither_is_reachable_over_the_network() {
        for e in module().capabilities {
            let id = e.capability.id.as_str().to_string();
            let x = &e.capability.exposure;
            if id == "evidence.record" || id == "evidence.publish" {
                assert!(
                    !e.capability.kind.is_read_only(),
                    "{id} is declared a query"
                );
                assert!(x.mcp.is_none(), "{id} is reachable over MCP");
                assert!(x.http.is_none(), "{id} is reachable over HTTP");
                assert_eq!(
                    e.capability.visibility,
                    crate::capability::Visibility::Developer,
                    "{id} must be a developer capability"
                );
                assert!(
                    matches!(e.capability.benchmark, BenchmarkPolicy::Waived { .. }),
                    "{id} must not be timed in a loop: it writes a file"
                );
            } else if id == "evidence.stamp" {
                // a read, but of a path its caller names: a network client must not choose a
                // file this executable reads, so it is offered only on the command line; and
                // it is waived, because the benchmark repository is not a git work tree
                assert!(e.capability.kind.is_read_only(), "{id} writes");
                assert!(x.mcp.is_none(), "{id} is reachable over MCP");
                assert!(x.http.is_none(), "{id} is reachable over HTTP");
                assert_eq!(
                    e.capability.visibility,
                    crate::capability::Visibility::Developer,
                    "{id} must be a developer capability"
                );
                assert!(
                    matches!(
                        e.capability.benchmark,
                        BenchmarkPolicy::Waived {
                            reason: WaiverReason::ExternalDependency
                        }
                    ),
                    "{id} is waived for its external git"
                );
            } else {
                assert!(e.capability.kind.is_read_only(), "{id} writes");
                assert!(
                    x.mcp.is_some() && x.http.is_some(),
                    "{id} lost a projection"
                );
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

    /// A tree state means something only about a presented commit, and a revision the
    /// repository cannot name is refused with its name rather than judged at something else.
    #[test]
    fn a_presented_revision_is_checked_before_anything_is_judged() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        match ctx.execute(
            "evidence.report",
            serde_json::json!({ "presented_tree": "dirty" }),
        ) {
            Err(CapabilityError::InvalidInput(m)) => {
                assert!(m.contains("presented"), "{m}")
            }
            other => panic!("a tree state with no commit was accepted: {other:?}"),
        }
        // the benchmark repository is not a git repository, so no revision resolves there
        match ctx.execute(
            "evidence.report",
            serde_json::json!({ "presented": "HEAD", "presented_tree": "clean" }),
        ) {
            Err(CapabilityError::InvalidInput(m)) => assert!(m.contains("HEAD"), "{m}"),
            other => panic!("a revision that names nothing was judged: {other:?}"),
        }
        // and the working tree is still the default
        let v = ctx
            .execute("evidence.report", serde_json::json!({}))
            .unwrap();
        assert_eq!(v["presented"]["revision"], "working_tree");
    }

    /// What a crate run held that no claim can name yet reaches the recording's answer, in
    /// the order the output held it, and a recording with nothing dropped carries no
    /// `dropped` at all, so a suite-only recording answers as it did before the list existed.
    #[test]
    fn the_recording_answers_with_what_it_dropped() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(repo.root())
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "fixture"]);
        let ctx = repo.context().unwrap();

        let reports = tempfile::tempdir().unwrap();
        let log = reports.path().join("crate.log");
        std::fs::write(
            &log,
            "     Running unittests src/lib.rs (target/debug/deps/majordomus_cli-1)\n\
             test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n\
                Doc-tests majordomus_cli\n\
             test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
        )
        .unwrap();
        let v = ctx
            .execute(
                "evidence.record",
                serde_json::json!({ "crate_output": log.to_string_lossy() }),
            )
            .unwrap();
        assert_eq!(v["recorded"], 0, "{v}");
        let dropped: Vec<(&str, &str)> = v["dropped"]
            .as_array()
            .unwrap_or_else(|| panic!("no dropped list: {v}"))
            .iter()
            .map(|d| {
                (
                    d["producer"].as_str().unwrap_or("?"),
                    d["what"].as_str().unwrap_or("?"),
                )
            })
            .collect();
        assert_eq!(
            dropped,
            [
                ("crate", "unittests src/lib.rs"),
                ("crate", "doc-tests majordomus_cli")
            ]
        );

        let tsv = reports.path().join("run.tsv");
        std::fs::write(&tsv, "99_ghost\tok\t1\tparallel\n").unwrap();
        let v = ctx
            .execute(
                "evidence.record",
                serde_json::json!({ "suite": tsv.to_string_lossy() }),
            )
            .unwrap();
        assert!(v.get("dropped").is_none(), "{v}");
    }

    /// Each ledger a report reads — the one a presented commit holds, the working copy
    /// beside it, and the working tree's own — is refused when it cannot be read, rather
    /// than read as a ledger that recorded nothing.
    #[test]
    fn an_unreadable_ledger_is_refused_at_every_revision() {
        use crate::evidence::LEDGER_PATH;
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(repo.root())
                .args(args)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "fixture"]);
        let ctx = repo.context().unwrap();
        let at_head = serde_json::json!({ "presented": "HEAD" });

        let v = ctx.execute("evidence.report", at_head.clone()).unwrap();
        assert_eq!(v["presented"]["revision"], git(&["rev-parse", "HEAD"]));
        assert_eq!(v["presented"]["tree"], "clean");

        let refused = |input: serde_json::Value| match ctx.execute("evidence.report", input) {
            Err(CapabilityError::Internal(m)) => m,
            other => panic!("an unreadable ledger was judged past: {other:?}"),
        };
        // the working copy cannot be read: a run it may hold cannot be ruled out
        std::fs::create_dir_all(repo.root().join(".ai/repo/evidence")).unwrap();
        std::fs::write(repo.root().join(LEDGER_PATH), "not a ledger").unwrap();
        assert!(refused(at_head.clone()).contains("cannot be ruled out"));
        assert!(refused(serde_json::json!({})).contains(LEDGER_PATH));
        // and the ledger the commit holds cannot be read either
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "an unreadable ledger"]);
        assert!(refused(at_head).contains(LEDGER_PATH));
    }

    /// The small repository, committed, with the local layer ignored as `init` ignores it;
    /// returns the repository and its HEAD.
    fn committed_repo() -> (crate::synthetic::SyntheticRepository, String) {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(repo.root())
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        std::fs::write(repo.root().join(".gitignore"), ".ai/local/\n").unwrap();
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "fixture"]);
        let head = git(&["rev-parse", "HEAD"]);
        (repo, head)
    }

    /// A run record measured at `commit` on `tree`, carrying a coverage measurement of
    /// `coverage` (its commit and tree) when one is given.
    fn run_record(
        id: &str,
        commit: &str,
        tree: &str,
        coverage: Option<(&str, &str)>,
    ) -> serde_json::Value {
        let one = serde_json::json!({ "covered": 1, "total": 2, "percent": 50.0 });
        let dims = serde_json::json!({ "lines": one, "functions": one, "regions": one });
        let mut run = serde_json::json!({
            "schema": 1, "id": id, "origin": "local", "commit": commit,
            "working_tree": tree, "recorded_at": "2026-10-22T12:00:00Z", "ledger": "local"
        });
        if let Some((at, measured)) = coverage {
            run["coverage"] = serde_json::json!({
                "commit": at, "working_tree": measured, "measurement": "scripts/rust-coverage",
                "test_code": "excluded", "crate": dims,
                "domain": { "lines": one, "functions": one, "regions": one,
                            "files": [], "missing": [] },
                "files": {}, "floors": { "crate_lines": 90.0 }
            });
        }
        run
    }

    /// Every subject kind answers through the subject index's own judge, an unknown key is a
    /// not-found that says where the keys are, and a tree state with no commit is refused.
    #[test]
    fn a_subject_answers_through_the_index_and_an_unknown_one_is_not_found() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let ask = |input: serde_json::Value| ctx.execute("evidence.subject", input);

        let v = ask(serde_json::json!({ "subject": "capability:evidence.subject" })).unwrap();
        assert_eq!(v["kind"], "capability", "{v}");
        assert_eq!(v["presented"]["revision"], "working_tree");
        assert!(v["totals"].is_object() && v["members"].is_array(), "{v}");
        let tool =
            ask(serde_json::json!({ "subject": "mcp:majordomus_evidence_subject" })).unwrap();
        assert_eq!(tool["members"][0]["subject"], "capability:evidence.subject");

        match ask(serde_json::json!({ "subject": "feature:no-such" })) {
            Err(CapabilityError::NotFound(m)) => {
                assert!(m.contains("`feature:no-such` is not a subject"), "{m}");
                assert!(m.contains("evidence-subjects.json"), "{m}");
            }
            other => panic!("an unknown subject was answered: {other:?}"),
        }
        match ask(serde_json::json!({ "subject": "claim:x", "presented_tree": "dirty" })) {
            Err(CapabilityError::InvalidInput(m)) => assert!(m.contains("presented"), "{m}"),
            other => panic!("a tree state with no commit was accepted: {other:?}"),
        }
        // the benchmark repository is not a git repository, so no revision resolves there
        match ask(serde_json::json!({ "subject": "claim:x", "presented": "HEAD" })) {
            Err(CapabilityError::InvalidInput(m)) => assert!(m.contains("HEAD"), "{m}"),
            other => panic!("a revision that names nothing was judged: {other:?}"),
        }
    }

    /// At a presented commit a subject is judged as the claim report judges it: the revision
    /// is the commit, and a dirty tree the caller knows of weakens the measured one.
    #[test]
    fn a_subject_is_judged_at_a_presented_commit_as_the_report_is() {
        let (repo, head) = committed_repo();
        let ctx = repo.context().unwrap();
        let key = SubjectKind::Capability.key("evidence.report");
        let v = ctx
            .execute(
                "evidence.subject",
                serde_json::json!({ "subject": key, "presented": "HEAD", "presented_tree": "dirty" }),
            )
            .unwrap();
        assert_eq!(v["presented"]["revision"], head.as_str(), "{v}");
        assert_eq!(v["presented"]["tree"], "dirty", "{v}");
        let report = ctx
            .execute(
                "evidence.report",
                serde_json::json!({ "presented": "HEAD", "presented_tree": "dirty" }),
            )
            .unwrap();
        assert_eq!(v["presented"], report["presented"]);
    }

    /// Coverage is current only for its own commit measured clean with the checkout clean;
    /// a record without coverage is counted, and a file that is not a run record is named.
    #[test]
    fn coverage_is_current_only_for_its_own_commit_on_a_clean_tree() {
        let (repo, head) = committed_repo();
        let other = "f".repeat(40);
        let runs = repo.root().join(RUNS_DIR);
        std::fs::create_dir_all(&runs).unwrap();
        let write = |name: &str, v: &serde_json::Value| {
            std::fs::write(runs.join(name), serde_json::to_string(v).unwrap()).unwrap()
        };
        write(
            "a-current.json",
            &run_record("r-a", &head, "clean", Some((head.as_str(), "clean"))),
        );
        write(
            "b-other.json",
            &run_record("r-b", &other, "clean", Some((other.as_str(), "clean"))),
        );
        write(
            "c-dirty.json",
            &run_record("r-c", &head, "dirty", Some((head.as_str(), "dirty"))),
        );
        write("d-bare.json", &run_record("r-d", &head, "clean", None));
        // a CI artifact's record.json carries the run record under `run_record`
        write(
            "e-recording.json",
            &serde_json::json!({ "recorded": 0,
                "run_record": run_record("r-e", &head, "clean", Some((head.as_str(), "unknown"))) }),
        );
        write(
            "f-manifest.json",
            &serde_json::json!({ "schema": 1, "commit": head }),
        );
        let mut later = run_record("r-g", &head, "clean", None);
        later["schema"] = serde_json::json!(2);
        write("g-later.json", &later);
        std::fs::write(runs.join("h-broken.json"), "{").unwrap();
        std::fs::write(runs.join("notes.txt"), "not read").unwrap();
        let ctx = repo.context().unwrap();

        let v = ctx
            .execute("evidence.coverage", serde_json::json!({}))
            .unwrap();
        assert_eq!(v["source"], RUNS_DIR, "{v}");
        assert_eq!(v["present"], true);
        assert_eq!(v["presented"]["commit"], head.as_str());
        assert_eq!(v["presented"]["tree"], "clean");
        assert_eq!(v["runs"], 5, "four with coverage and one without: {v}");
        let states: Vec<(&str, &str)> = v["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (r["run"].as_str().unwrap(), r["state"].as_str().unwrap()))
            .collect();
        assert_eq!(
            states,
            [
                ("r-a", "current"),
                ("r-b", "stale"),
                ("r-c", "stale"),
                ("r-e", "stale")
            ]
        );
        let reason = |i: usize| v["records"][i]["reason"].as_str().unwrap().to_string();
        assert!(reason(1).contains("not at the presented commit"), "{v}");
        assert!(reason(2).contains("measured its tree as dirty"), "{v}");
        assert!(reason(3).contains("measured its tree as unknown"), "{v}");
        assert_eq!(v["records"][0]["below_floor"], serde_json::json!(["crate"]));
        assert_eq!(
            v["records"][0]["file"],
            format!("{RUNS_DIR}/a-current.json")
        );
        let unread: Vec<&str> = v["unread"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| u["file"].as_str().unwrap())
            .collect();
        assert_eq!(
            unread,
            [
                format!("{RUNS_DIR}/f-manifest.json"),
                format!("{RUNS_DIR}/g-later.json"),
                format!("{RUNS_DIR}/h-broken.json"),
            ]
        );
        assert!(v["unread"][1]["reason"]
            .as_str()
            .unwrap()
            .contains("schema 2"));

        // what the caller knows of the tree weakens it, and a checkout that is dirty is dirty
        let weakened = ctx
            .execute(
                "evidence.coverage",
                serde_json::json!({ "presented_tree": "dirty" }),
            )
            .unwrap();
        assert_eq!(weakened["records"][0]["state"], "stale");
        assert!(weakened["records"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("the presented tree is dirty"));
        std::fs::write(repo.root().join("untracked.txt"), "x").unwrap();
        let dirty = ctx
            .execute("evidence.coverage", serde_json::json!({}))
            .unwrap();
        assert_eq!(dirty["presented"]["tree"], "dirty");
        assert_eq!(dirty["records"][0]["state"], "stale");
    }

    /// A named source is read inside the checkout only: one file reads alone, a path that is
    /// not there is a not-found, and a path outside the checkout is refused.
    #[test]
    fn a_named_source_is_read_inside_the_checkout_only() {
        let (repo, head) = committed_repo();
        let ctx = repo.context().unwrap();
        let none = ctx
            .execute("evidence.coverage", serde_json::json!({}))
            .unwrap();
        assert_eq!(none["present"], false, "nothing wrote the local runs yet");
        assert_eq!(none["runs"], 0);

        let dir = repo.root().join(".ai/local/ci");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("record.json");
        std::fs::write(
            &file,
            serde_json::to_string(&run_record(
                "r",
                &head,
                "clean",
                Some((head.as_str(), "clean")),
            ))
            .unwrap(),
        )
        .unwrap();
        for from in [
            ".ai/local/ci/record.json".to_string(),
            file.display().to_string(),
        ] {
            let v = ctx
                .execute("evidence.coverage", serde_json::json!({ "from": from }))
                .unwrap();
            assert_eq!(v["source"], ".ai/local/ci/record.json", "{v}");
            assert_eq!(v["records"][0]["state"], "current", "{v}");
        }
        match ctx.execute(
            "evidence.coverage",
            serde_json::json!({ "from": "no/such/run.json" }),
        ) {
            Err(CapabilityError::NotFound(m)) => assert!(m.contains("no/such/run.json"), "{m}"),
            other => panic!("a source that is not there was read: {other:?}"),
        }
        let outside = tempfile::tempdir().unwrap();
        for from in [outside.path().display().to_string(), "..".to_string()] {
            match ctx.execute("evidence.coverage", serde_json::json!({ "from": from })) {
                Err(CapabilityError::Refused(m)) => assert!(m.contains("outside"), "{m}"),
                other => panic!("a source outside the checkout was read: {other:?}"),
            }
        }
    }

    /// Without a commit to present, no coverage is current, and the answer says why.
    #[test]
    fn coverage_without_a_commit_is_never_current() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let runs = repo.root().join(RUNS_DIR);
        std::fs::create_dir_all(&runs).unwrap();
        let c = "c".repeat(40);
        std::fs::write(
            runs.join("one.json"),
            serde_json::to_string(&run_record("r", &c, "clean", Some((c.as_str(), "clean"))))
                .unwrap(),
        )
        .unwrap();
        let ctx = repo.context().unwrap();
        let v = ctx
            .execute("evidence.coverage", serde_json::json!({}))
            .unwrap();
        assert!(v["presented"]["commit"].is_null(), "{v}");
        assert_eq!(v["presented"]["tree"], "unknown");
        assert_eq!(v["records"][0]["state"], "stale");
        assert!(v["records"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("no commit is presented"));
    }

    /// A publication is written outside the checkout only, from the run records of a named
    /// source, and the file holds exactly the publication the answer carries; a source that
    /// does not exist is a not-found, and a destination that is a file is refused.
    #[test]
    fn a_publication_is_written_outside_the_checkout_only() {
        let (repo, head) = committed_repo();
        let ctx = repo.context().unwrap();
        let publish = |input: serde_json::Value| ctx.execute("evidence.publish", input);
        for inside in [".", "sub/dir", "sub/../other"] {
            match publish(serde_json::json!({ "presented": "HEAD", "out": inside })) {
                Err(CapabilityError::Refused(m)) => {
                    assert!(m.contains("inside the checkout"), "{m}")
                }
                other => panic!("{inside} inside the checkout was accepted: {other:?}"),
            }
        }
        let away = tempfile::tempdir().unwrap();
        let from = away.path().join("artifact");
        std::fs::create_dir_all(&from).unwrap();
        let own = run_record("r-own", &head, "clean", None);
        std::fs::write(
            from.join("record.json"),
            serde_json::json!({ "run_record": own }).to_string(),
        )
        .unwrap();
        std::fs::write(from.join("manifest.json"), "{}").unwrap();
        let out = away.path().join("published");
        let v = publish(serde_json::json!({
            "presented": "HEAD",
            "from": from.to_string_lossy(),
            "out": out.to_string_lossy(),
        }))
        .unwrap();
        let p = &v["publication"];
        assert_eq!(p["state"], "current", "{v}");
        assert_eq!(p["records"][0]["relation"], "own", "{v}");
        assert_eq!(p["unread"][0]["file"], "manifest.json", "{v}");
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(out.join(publication::FILE)).unwrap())
                .unwrap();
        assert_eq!(&written, p, "the file is the answer");

        // one run record named by itself is read as the source
        let one = publish(serde_json::json!({
            "presented": "HEAD",
            "from": from.join("record.json").to_string_lossy(),
            "run": "pending",
            "out": out.to_string_lossy(),
        }))
        .unwrap();
        assert_eq!(one["publication"]["state"], "current", "{one}");

        match publish(serde_json::json!({
            "presented": "HEAD", "from": away.path().join("none").to_string_lossy(),
            "out": out.to_string_lossy(),
        })) {
            Err(CapabilityError::NotFound(m)) => assert!(m.contains("cannot be read"), "{m}"),
            other => panic!("a missing source was read: {other:?}"),
        }
        let file = out.join(publication::FILE);
        match publish(serde_json::json!({ "presented": "HEAD", "out": file.to_string_lossy() })) {
            Err(CapabilityError::Refused(m)) => assert!(m.contains("is a file"), "{m}"),
            other => panic!("a file was taken for a directory: {other:?}"),
        }
        match publish(serde_json::json!({ "presented": "no-such", "out": out.to_string_lossy() })) {
            Err(CapabilityError::InvalidInput(m)) => assert!(m.contains("no-such"), "{m}"),
            other => panic!("a revision that names nothing was published: {other:?}"),
        }
    }
}
